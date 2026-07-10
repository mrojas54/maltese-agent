import type { z } from "zod";

/**
 * Shared in-process fake for the MA-10 schema'd MCP client boundary.
 *
 * Test files substitute this module for `src/lib/mcp.js` via
 * `vi.mock("../../src/lib/mcp.js", () => import("../helpers/fakeMcp.js"))`
 * (the factory and the test file share this module instance, so the
 * `setMcpResponder`/`fakeMcp` controls below steer what the code under test
 * sees). The seam is the PUBLIC one: handlers keep constructing
 * `FalconMcpClient` and calling `.call(tool, args, schema)` exactly as in
 * production, and the fake still runs every canned payload through the REAL
 * zod schema passed at the call site — so toolSchemas stays load-bearing
 * and a drifted fixture fails the same way a drifted wire shape would
 * (McpValidationError, mirroring src/lib/mcp.ts).
 *
 * Failure simulation: a responder may throw (or return an Error, which the
 * fake throws) to model the execution-error path (`tool X returned error`),
 * letting tests pin the no-silent-catch contract (AC-19).
 */

export interface RecordedCall {
  tool: string;
  args: Record<string, unknown>;
}

export type McpResponder = (
  tool: string,
  args: Record<string, unknown>,
  callIndex: number,
) => unknown;

interface FakeMcpState {
  responder: McpResponder | null;
  calls: RecordedCall[];
  instances: FalconMcpClient[];
}

/** Inspection surface for tests: every call and client instance, in order. */
export const fakeMcp: FakeMcpState = {
  responder: null,
  calls: [],
  instances: [],
};

/** Install the scenario script for the current test. */
export function setMcpResponder(responder: McpResponder): void {
  fakeMcp.responder = responder;
}

/** Reset between tests (call from beforeEach). */
export function resetFakeMcp(): void {
  fakeMcp.responder = null;
  fakeMcp.calls = [];
  fakeMcp.instances = [];
}

/** Mirrors src/lib/mcp.ts McpValidationError closely enough for assertions. */
export class McpValidationError extends Error {
  readonly toolName: string;
  readonly issues: readonly z.core.$ZodIssue[];

  constructor(toolName: string, issues: readonly z.core.$ZodIssue[]) {
    const mismatches = issues
      .map((i) => `${i.path.join(".") || "(root)"}: ${i.message}`)
      .join("; ");
    super(`tool ${toolName} result failed schema validation: ${mismatches}`);
    this.name = "McpValidationError";
    this.toolName = toolName;
    this.issues = issues;
  }
}

export interface McpOptions {
  binary: string;
  root: string;
  readOnly?: boolean;
  enableExec?: boolean;
  env?: Record<string, string>;
}

export class FalconMcpClient {
  connected = false;
  closed = false;
  /** Calls made through THIS instance (fakeMcp.calls has the global order). */
  readonly calls: RecordedCall[] = [];

  constructor(readonly opts: McpOptions) {
    fakeMcp.instances.push(this);
  }

  async connect(): Promise<void> {
    this.connected = true;
  }

  async call<T>(
    toolName: string,
    args: Record<string, unknown>,
    schema: z.ZodType<T>,
  ): Promise<T> {
    if (!this.connected || this.closed)
      throw new Error("not connected; call connect() first");
    const index = fakeMcp.calls.length;
    const record = { tool: toolName, args };
    fakeMcp.calls.push(record);
    this.calls.push(record);
    if (!fakeMcp.responder)
      throw new Error(
        `fakeMcp: no responder installed (tool ${toolName} called)`,
      );
    const result = fakeMcp.responder(toolName, args, index);
    if (result instanceof Error) throw result;
    const parsed = schema.safeParse(result);
    if (!parsed.success) {
      throw new McpValidationError(toolName, parsed.error.issues);
    }
    return parsed.data;
  }

  async close(): Promise<void> {
    this.closed = true;
    this.connected = false;
  }
}
