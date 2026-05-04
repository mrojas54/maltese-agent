import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

export interface McpOptions {
  binary: string;          // path to falcon-mcp binary
  root: string;            // sandbox root (the worktree path)
  readOnly?: boolean;
  enableExec?: boolean;
}

export class FalconMcpClient {
  private client: Client | null = null;
  private transport: StdioClientTransport | null = null;

  constructor(private opts: McpOptions) {}

  async connect(): Promise<void> {
    const args = ["--stdio", "--root", this.opts.root];
    if (this.opts.readOnly) args.push("--read-only");
    if (this.opts.enableExec) args.push("--enable-exec");

    this.transport = new StdioClientTransport({
      command: this.opts.binary,
      args,
      // StdioClientTransport's default env is a curated allowlist (PATH, HOME,
      // etc.) — NOT the full process.env. If falcon-mcp ever reads anything
      // outside that allowlist (e.g. RUST_LOG, custom config paths), pass it
      // explicitly via env: { ...process.env as Record<string, string> }.
    });
    this.client = new Client({ name: "falcon-detective", version: "0.1.0" }, { capabilities: {} });
    await this.client.connect(this.transport);
  }

  async call<T = unknown>(toolName: string, args: Record<string, unknown>): Promise<T> {
    if (!this.client) throw new Error("not connected; call connect() first");
    // The MCP SDK's default per-request timeout is 60s, which trips on cold
    // cargo compiles in fresh worktrees. cargo_check/test/clippy can take
    // several minutes when deps are uncached. 5 minutes leaves margin
    // without masking real hangs.
    const result = await this.client.callTool(
      { name: toolName, arguments: args },
      undefined,
      { timeout: 300_000 },
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
