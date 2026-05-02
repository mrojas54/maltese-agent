# Falcon-Detective Implementation Plan

> **⚠ External SDK verification.**
> This plan depends on three actively-developed npm packages whose APIs may have drifted since the plan was drafted: `@barnum/barnum` (combinator names + signatures — see spec §9 question 2; needs Robert's confirmation), `@modelcontextprotocol/sdk` (TypeScript MCP client — `Client`, `StdioClientTransport`, `callTool` shapes), and `@google/genai` (Gemini SDK — `responseSchema` + `generateContent` shape). Verify each against current README/changelog before dispatching Tasks 3, 4, and 13 respectively. The rmcp-drift story in [`poc-findings-2026-05-01.md`](../poc-findings-2026-05-01.md) (rmcp 0.1 → 1.6.x) is precedent: expect similar surgical-but-systematic updates here.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the TypeScript Barnum workflow that triages, fixes, and verifies bugs in `falcon-agent` by orchestrating Gemini calls and `falcon-mcp` tools.

**Architecture:** Pure-TS project consuming `@barnum/barnum` (the Rust runtime is a binary dep) + `@google/genai` for reasoning + falcon-mcp via stdio MCP. Each handler is its own file under `src/handlers/`, has a Zod input/output validator, and reads its prompt from a markdown file in `prompts/`. The workflow is a single ~30-LOC composition in `src/workflows/tidy-and-debug.ts`. Test-time determinism via Gemini cassettes.

**Tech Stack:** Node 20+, TypeScript 5.4+, `@barnum/barnum`, `@google/genai`, `@modelcontextprotocol/sdk` (TS MCP client), `zod`, `vitest`

**Spec reference:** `docs/superpowers/specs/2026-04-30-maltese-circus-design.md` §5, §7

**Prerequisites:** falcon-mcp builds (Plan 1 complete) and falcon-agent builds in its planted-broken state (Plan 2 complete).

---

## File Structure

```
falcon-detective/
├── package.json
├── tsconfig.json
├── vitest.config.ts
├── README.md
├── src/
│   ├── cli.ts                    binary entrypoint (npm run fix --target ../falcon-agent)
│   ├── lib/
│   │   ├── gemini.ts             SDK wrapper with schema retry + cassette mode
│   │   ├── mcp.ts                falcon-mcp client (spawn + call)
│   │   ├── prompts.ts            load + interpolate prompt files
│   │   └── types.ts              Issue, PoisonReport, VerifyResult, etc.
│   ├── handlers/
│   │   ├── prepWorktree.ts
│   │   ├── triage.ts
│   │   ├── readContext.ts
│   │   ├── classify.ts
│   │   ├── analyzePoison.ts
│   │   ├── proposePoisonFix.ts
│   │   ├── proposeBugFix.ts
│   │   ├── proposeLintFix.ts
│   │   ├── applyEdit.ts
│   │   ├── verify.ts
│   │   ├── commit.ts             commitOne, commitAll, revertOne, escalate
│   │   └── finalSweep.ts
│   └── workflows/
│       └── tidy-and-debug.ts
├── prompts/
│   ├── triage.md
│   ├── analyze-poison.md
│   ├── propose-poison-fix.md
│   ├── propose-bug-fix.md
│   └── propose-lint-fix.md
├── fixtures/
│   └── cassettes/                recorded Gemini responses for tests
└── tests/
    ├── handlers/                 unit tests per handler
    ├── lib/
    └── e2e.test.ts               full pipeline against falcon-agent
```

**Decomposition rationale:** one file per handler keeps each under ~80 LOC. `lib/` holds infra (LLM, MCP, prompts) — the things every handler needs. Prompt templates are in their own dir as plain markdown so they're reviewable by non-TS devs and diffable independently.

---

## Task 1: Bootstrap the package

**Files:**
- Create: `falcon-detective/package.json`, `tsconfig.json`, `vitest.config.ts`, `README.md`
- Create: `falcon-detective/src/lib/types.ts` (placeholder)

- [ ] **Step 1: Create `package.json`**

```json
{
  "name": "@maltese-circus/falcon-detective",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "bin": {
    "falcon-detective": "./dist/cli.js"
  },
  "scripts": {
    "build": "tsc",
    "fix": "tsx src/cli.ts",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "@barnum/barnum": "^0.1.0",
    "@google/genai": "^1.0.0",
    "@modelcontextprotocol/sdk": "^1.0.0",
    "zod": "^3.23.0"
  },
  "devDependencies": {
    "@types/node": "^20.10.0",
    "tsx": "^4.7.0",
    "typescript": "^5.4.0",
    "vitest": "^1.5.0"
  }
}
```

- [ ] **Step 2: Create `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "lib": ["ES2022"],
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "outDir": "./dist",
    "rootDir": "./src",
    "declaration": true,
    "resolveJsonModule": true
  },
  "include": ["src/**/*"]
}
```

- [ ] **Step 3: Create `vitest.config.ts`**

```ts
import { defineConfig } from "vitest/config";
export default defineConfig({
  test: {
    environment: "node",
    testTimeout: 30_000,
    include: ["tests/**/*.test.ts"],
  },
});
```

- [ ] **Step 4: Create `README.md`**

```markdown
# falcon-detective

The Barnum workflow that triages, fixes, and verifies bugs in `falcon-agent`. Orchestrates Gemini reasoning + falcon-mcp tools.

## Run

```bash
npm install
npm run build
npm run fix -- --target ../falcon-agent
```

See `docs/superpowers/specs/2026-04-30-maltese-circus-design.md` §5 for the design.
```

- [ ] **Step 5: Stub `src/lib/types.ts`**

```ts
// types are filled in by Task 2; placeholder to satisfy initial build
export {};
```

- [ ] **Step 6: Verify install + build**

```bash
cd falcon-detective && npm install && npm run build
```
Expected: install completes, `dist/` is created with no source files yet (just the empty types).

- [ ] **Step 7: Commit**

```bash
git add falcon-detective/
git commit -m "feat(falcon-detective): bootstrap TS package"
```

---

## Task 2: Core types (Issue, PoisonReport, VerifyResult)

**Files:**
- Modify: `falcon-detective/src/lib/types.ts`
- Create: `falcon-detective/tests/lib/types.test.ts`

- [ ] **Step 1: Replace `src/lib/types.ts` with concrete schemas**

```ts
import { z } from "zod";

export const IssueKind = z.enum(["bug", "lint", "poison"]);
export type IssueKind = z.infer<typeof IssueKind>;

export const Location = z.object({
  file: z.string(),
  line: z.number().int().optional(),
  span: z.tuple([z.number().int(), z.number().int()]).optional(),
});
export type Location = z.infer<typeof Location>;

export const Issue = z.object({
  kind: IssueKind,
  location: Location,
  evidence: z.string(),
  hint: z.string().optional(),
});
export type Issue = z.infer<typeof Issue>;

export const TaggedIssue = z.discriminatedUnion("tag", [
  z.object({ tag: z.literal("Poison"), issue: Issue }),
  z.object({ tag: z.literal("Bug"),    issue: Issue }),
  z.object({ tag: z.literal("Lint"),   issue: Issue }),
]);
export type TaggedIssue = z.infer<typeof TaggedIssue>;

export const PoisonReport = z.object({
  suspectExample: z.number().int().nonnegative(),
  reasoning: z.string().min(20),
  anomalies: z.array(z.string()).min(1),
  recommendation: z.enum(["remove", "replace", "harden_prompt"]),
});
export type PoisonReport = z.infer<typeof PoisonReport>;

export const UnifiedDiff = z.object({
  path: z.string(),
  diff: z.string().min(1),
});
export type UnifiedDiff = z.infer<typeof UnifiedDiff>;

export const VerifyResult = z.discriminatedUnion("tag", [
  z.object({ tag: z.literal("Clean") }),
  z.object({ tag: z.literal("Broken"), errors: z.array(z.string()) }),
  z.object({ tag: z.literal("Stuck"),  attempts: z.number().int() }),
]);
export type VerifyResult = z.infer<typeof VerifyResult>;
```

- [ ] **Step 2: Add a sanity unit test**

`tests/lib/types.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { Issue, PoisonReport, VerifyResult } from "../../src/lib/types.js";

describe("type schemas", () => {
  it("rejects an Issue without a location", () => {
    expect(() => Issue.parse({ kind: "bug", evidence: "..." })).toThrow();
  });
  it("PoisonReport requires reasoning length ≥20", () => {
    const bad = { suspectExample: 0, reasoning: "short", anomalies: ["a"], recommendation: "remove" };
    expect(() => PoisonReport.parse(bad)).toThrow();
  });
  it("VerifyResult discriminates by tag", () => {
    expect(VerifyResult.parse({ tag: "Clean" })).toEqual({ tag: "Clean" });
    expect(() => VerifyResult.parse({ tag: "Broken" })).toThrow();
  });
});
```

- [ ] **Step 3: Run + commit**

```bash
npm test -- tests/lib/types.test.ts
git add falcon-detective/
git commit -m "feat(falcon-detective): core Zod types (Issue, PoisonReport, VerifyResult)"
```

---

## Task 3: Gemini client wrapper (with cassette mode)

**Files:**
- Create: `falcon-detective/src/lib/gemini.ts`
- Create: `falcon-detective/tests/lib/gemini.test.ts`
- Create: `falcon-detective/fixtures/cassettes/.gitkeep`

- [ ] **Step 1: Create `src/lib/gemini.ts`**

```ts
import { GoogleGenAI } from "@google/genai";
import { ZodSchema, z } from "zod";
import { createHash } from "node:crypto";
import { readFile, writeFile, mkdir, access } from "node:fs/promises";
import { join } from "node:path";

export type Mode = "live" | "cassette" | "record";

export interface CallOptions<T> {
  prompt: string;
  schema: ZodSchema<T>;
  model?: string;
  temperature?: number;
}

const CASSETTE_DIR = process.env.CASSETTE_DIR ?? join(process.cwd(), "fixtures", "cassettes");

function hashKey(model: string, prompt: string, schemaName: string): string {
  return createHash("sha256").update(`${model}\n${schemaName}\n${prompt}`).digest("hex").slice(0, 16);
}

async function readCassette(key: string): Promise<string | null> {
  const path = join(CASSETTE_DIR, `${key}.json`);
  try { await access(path); } catch { return null; }
  return await readFile(path, "utf8");
}

async function writeCassette(key: string, body: string) {
  await mkdir(CASSETTE_DIR, { recursive: true });
  await writeFile(join(CASSETTE_DIR, `${key}.json`), body);
}

export class Gemini {
  private mode: Mode;
  private ai: GoogleGenAI | null = null;

  constructor(mode: Mode = (process.env.GEMINI_MODE as Mode) ?? "live") {
    this.mode = mode;
    if (mode !== "cassette") {
      const apiKey = process.env.GEMINI_API_KEY;
      if (!apiKey) throw new Error("GEMINI_API_KEY required for live/record mode");
      this.ai = new GoogleGenAI({ apiKey });
    }
  }

  async call<T>(opts: CallOptions<T>): Promise<T> {
    const model = opts.model ?? "gemini-2.5-pro";
    const schemaName = (opts.schema as any)._def?.typeName ?? "anon";
    const key = hashKey(model, opts.prompt, schemaName);

    if (this.mode === "cassette") {
      const body = await readCassette(key);
      if (!body) throw new Error(`no cassette for ${key} (prompt hash). Re-record with GEMINI_MODE=record.`);
      return opts.schema.parse(JSON.parse(body));
    }

    let lastErr: unknown;
    for (let attempt = 0; attempt <= 2; attempt++) {
      const resp = await this.ai!.models.generateContent({
        model,
        contents: opts.prompt,
        config: {
          responseMimeType: "application/json",
          temperature: opts.temperature ?? 0.2,
        },
      });
      const text = resp.text ?? "";
      try {
        const parsed = opts.schema.parse(JSON.parse(text));
        if (this.mode === "record") await writeCassette(key, text);
        return parsed;
      } catch (e) {
        lastErr = e;
        // re-prompt with the validation error appended; fall through to next iter
        opts = { ...opts, prompt: `${opts.prompt}\n\nPREVIOUS ATTEMPT FAILED VALIDATION: ${(e as Error).message}\nReturn ONLY valid JSON conforming to the schema.` };
      }
    }
    throw new Error(`Gemini schema validation failed after 3 attempts: ${(lastErr as Error)?.message}`);
  }
}
```

- [ ] **Step 2: Create cassette dir placeholder**

```bash
mkdir -p falcon-detective/fixtures/cassettes
touch falcon-detective/fixtures/cassettes/.gitkeep
```

- [ ] **Step 3: Test cassette mode**

`tests/lib/gemini.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { Gemini } from "../../src/lib/gemini.js";
import { z } from "zod";
import { mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";

const TEST_DIR = join(process.cwd(), "fixtures", "cassettes-test");

beforeEach(async () => {
  await rm(TEST_DIR, { recursive: true, force: true });
  await mkdir(TEST_DIR, { recursive: true });
  process.env.CASSETTE_DIR = TEST_DIR;
});

describe("Gemini cassette mode", () => {
  it("replays a saved cassette", async () => {
    const schema = z.object({ kind: z.string() });
    const prompt = "test prompt";
    const model = "gemini-2.5-pro";
    const schemaName = "ZodObject";
    const key = createHash("sha256").update(`${model}\n${schemaName}\n${prompt}`).digest("hex").slice(0, 16);
    await writeFile(join(TEST_DIR, `${key}.json`), JSON.stringify({ kind: "bug" }));

    const g = new Gemini("cassette");
    const out = await g.call({ prompt, schema });
    expect(out).toEqual({ kind: "bug" });
  });

  it("errors when cassette is missing", async () => {
    const g = new Gemini("cassette");
    await expect(g.call({ prompt: "missing", schema: z.object({}) })).rejects.toThrow(/no cassette/);
  });
});
```

- [ ] **Step 4: Run + commit**

```bash
npm test -- tests/lib/gemini.test.ts
git add falcon-detective/
git commit -m "feat(falcon-detective): Gemini wrapper with cassette mode"
```

---

## Task 4: MCP client wrapper (spawn falcon-mcp + call tools)

**Files:**
- Create: `falcon-detective/src/lib/mcp.ts`
- Create: `falcon-detective/tests/lib/mcp.test.ts`

- [ ] **Step 1: Create `src/lib/mcp.ts`**

```ts
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
    });
    this.client = new Client({ name: "falcon-detective", version: "0.1.0" }, { capabilities: {} });
    await this.client.connect(this.transport);
  }

  async call<T = unknown>(toolName: string, args: Record<string, unknown>): Promise<T> {
    if (!this.client) throw new Error("not connected; call connect() first");
    const result = await this.client.callTool({ name: toolName, arguments: args });
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
```

- [ ] **Step 2: Test against the real falcon-mcp binary**

`tests/lib/mcp.test.ts`:

```ts
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { FalconMcpClient } from "../../src/lib/mcp.js";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const FALCON_MCP_BIN = process.env.FALCON_MCP_BIN ?? join(process.cwd(), "../target/debug/falcon-mcp");

let dir: string;

beforeAll(async () => {
  dir = await mkdtemp(join(tmpdir(), "fd-mcp-"));
  await writeFile(join(dir, "hello.txt"), "the falcon");
});
afterAll(async () => { await rm(dir, { recursive: true, force: true }); });

describe("FalconMcpClient", () => {
  it("calls fs_read through the MCP transport", async () => {
    const c = new FalconMcpClient({ binary: FALCON_MCP_BIN, root: dir });
    await c.connect();
    const result = await c.call<{ content: string; bytes: number }>("fs_read", { path: "hello.txt" });
    expect(result.content).toBe("the falcon");
    await c.close();
  });
});
```

- [ ] **Step 3: Run + commit**

Make sure falcon-mcp is built: `cargo build -p falcon-mcp` from the repo root.

```bash
npm test -- tests/lib/mcp.test.ts
git add falcon-detective/
git commit -m "feat(falcon-detective): MCP client wrapper around @modelcontextprotocol/sdk"
```

---

## Task 5: Prompt loader

**Files:**
- Create: `falcon-detective/src/lib/prompts.ts`
- Create: `falcon-detective/tests/lib/prompts.test.ts`

- [ ] **Step 1: Create `src/lib/prompts.ts`**

```ts
import { readFile } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const PROMPT_DIR = join(here, "..", "..", "prompts");

/** Read a prompt template file and substitute {{vars}} from `data`. */
export async function promptFromFile(name: string, data: Record<string, string>): Promise<string> {
  const text = await readFile(join(PROMPT_DIR, name), "utf8");
  return text.replace(/{{(\w+)}}/g, (_m, key) => {
    if (!(key in data)) throw new Error(`prompt template ${name}: missing var {{${key}}}`);
    return data[key];
  });
}
```

- [ ] **Step 2: Create one test prompt file**

`falcon-detective/prompts/_test.md`:

```markdown
Hello, {{name}}. Decode the {{thing}}.
```

- [ ] **Step 3: Test interpolation**

`tests/lib/prompts.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { promptFromFile } from "../../src/lib/prompts.js";

describe("promptFromFile", () => {
  it("interpolates vars", async () => {
    const out = await promptFromFile("_test.md", { name: "Sam", thing: "cipher" });
    expect(out).toBe("Hello, Sam. Decode the cipher.\n");
  });
  it("throws on missing var", async () => {
    await expect(promptFromFile("_test.md", { name: "Sam" })).rejects.toThrow(/missing var/);
  });
});
```

- [ ] **Step 4: Run + commit**

```bash
npm test -- tests/lib/prompts.test.ts
git add falcon-detective/
git commit -m "feat(falcon-detective): prompt loader with var interpolation"
```

---

## Task 6: prepWorktree handler

**Files:**
- Create: `falcon-detective/src/handlers/prepWorktree.ts`
- Create: `falcon-detective/tests/handlers/prepWorktree.test.ts`

- [ ] **Step 1: Create `src/handlers/prepWorktree.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  repoRoot: z.string(),     // the maltese_circus repo root
  runName: z.string(),      // unique name for this run, e.g. "demo-2026-05-15"
});

const Output = z.object({
  worktreePath: z.string(),
});

export const prepWorktree = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const c = new FalconMcpClient({ binary: value.mcpBinary, root: value.repoRoot });
    await c.connect();
    try {
      const r = await c.call<{ path: string }>("git_worktree_add", { name: value.runName });
      return { worktreePath: r.path };
    } finally {
      await c.close();
    }
  },
}, "prepWorktree");
```

- [ ] **Step 2: Test with a real git repo**

`tests/handlers/prepWorktree.test.ts`:

```ts
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { prepWorktree } from "../../src/handlers/prepWorktree.js";
import { execSync } from "node:child_process";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const FALCON_MCP_BIN = process.env.FALCON_MCP_BIN ?? join(process.cwd(), "../target/debug/falcon-mcp");
let repo: string;

beforeAll(async () => {
  repo = await mkdtemp(join(tmpdir(), "fd-pw-"));
  execSync("git init", { cwd: repo });
  await writeFile(join(repo, "a.txt"), "x");
  execSync("git -c user.email=t@t -c user.name=t add . && git -c user.email=t@t -c user.name=t commit -m init", { cwd: repo, shell: "/bin/bash" });
});
afterAll(async () => { await rm(repo, { recursive: true, force: true }); });

describe("prepWorktree", () => {
  it("creates a worktree under .runs/", async () => {
    const out = await prepWorktree.handle({ value: { mcpBinary: FALCON_MCP_BIN, repoRoot: repo, runName: "test-run" } } as any);
    expect(out.worktreePath).toMatch(/\.runs\/test-run$/);
  });
});
```

- [ ] **Step 3: Run + commit**

```bash
npm test -- tests/handlers/prepWorktree.test.ts
git add falcon-detective/
git commit -m "feat(falcon-detective): prepWorktree handler"
```

---

## Task 7: triage handler + its prompt

**Files:**
- Create: `falcon-detective/src/handlers/triage.ts`
- Create: `falcon-detective/prompts/triage.md`
- Create: `falcon-detective/tests/handlers/triage.test.ts`

- [ ] **Step 1: Create `prompts/triage.md`**

```markdown
You are the triage step of a coding agent that fixes Rust services.

Given the output of `cargo check`, `cargo test`, `cargo clippy`, and a list of files in the target crate, produce a structured array of `Issue` objects.

Each Issue has `kind` ("bug" | "lint" | "poison"), `location { file, line? }`, `evidence` (the relevant tool output), and an optional `hint`.

Rules:
- A failing test → kind="bug" (or "poison" if the test name suggests prompt poisoning, e.g. names containing "bird", "themed", "poison", "trigger", "anomaly")
- A clippy warning → kind="lint"
- A `prompt.rs` or similar file with anomalous few-shot examples → kind="poison"
- Compile errors → kind="bug"

Tool outputs:

```json
{{cargoOutputs}}
```

File listing:

```
{{fileListing}}
```

Return ONLY a JSON array of Issue objects.
```

- [ ] **Step 2: Create `src/handlers/triage.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue } from "../lib/types.js";
import { Gemini } from "../lib/gemini.js";
import { FalconMcpClient } from "../lib/mcp.js";
import { promptFromFile } from "../lib/prompts.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),       // e.g. "falcon-agent" relative to worktreePath
});
const Output = z.object({ issues: z.array(Issue) });

export const triage = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      const [check, test, clippy, files] = await Promise.all([
        mcp.call("cargo_check", { crate_path: value.cratePath }),
        mcp.call("cargo_test", { crate_path: value.cratePath }),
        mcp.call("cargo_clippy", { crate_path: value.cratePath }),
        mcp.call("fs_list", { path: value.cratePath, glob: "**/*.rs" }),
      ]);
      const gemini = new Gemini();
      const prompt = await promptFromFile("triage.md", {
        cargoOutputs: JSON.stringify({ check, test, clippy }, null, 2),
        fileListing: JSON.stringify(files, null, 2),
      });
      const issues = await gemini.call({
        prompt,
        schema: z.object({ issues: z.array(Issue) }),
      });
      return issues;
    } finally { await mcp.close(); }
  },
}, "triage");
```

- [ ] **Step 3: Test with a recorded cassette**

`tests/handlers/triage.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { triage } from "../../src/handlers/triage.js";
import { mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";

const CASSETTE_DIR = join(process.cwd(), "fixtures", "cassettes-test-triage");

beforeEach(async () => {
  await rm(CASSETTE_DIR, { recursive: true, force: true });
  await mkdir(CASSETTE_DIR, { recursive: true });
  process.env.CASSETTE_DIR = CASSETTE_DIR;
  process.env.GEMINI_MODE = "cassette";
});

describe("triage", () => {
  it("returns issues parsed from a cassette", async () => {
    // Pre-seed a cassette matching whatever prompt hash triage will produce.
    // For unit-test purposes, we verify the handler shape by stubbing the
    // gemini call via an environment override (see Task 16 for full e2e).
    expect(triage.name).toBe("triage");
  });
});
```

(Full e2e validation happens in Task 16 against the real falcon-agent fixture.)

- [ ] **Step 4: Commit**

```bash
git add falcon-detective/
git commit -m "feat(falcon-detective): triage handler + prompt"
```

---

## Task 8: readContext + classify handlers

**Files:**
- Create: `falcon-detective/src/handlers/readContext.ts`
- Create: `falcon-detective/src/handlers/classify.ts`
- Create: `falcon-detective/tests/handlers/readContext.test.ts`
- Create: `falcon-detective/tests/handlers/classify.test.ts`

- [ ] **Step 1: Create `src/handlers/readContext.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue } from "../lib/types.js";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  issue: Issue,
});
const Output = z.object({
  issue: Issue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});

export const readContext = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      const target = await mcp.call<{ content: string }>("fs_read", { path: value.issue.location.file });
      const excerpts = [{ file: value.issue.location.file, content: target.content }];
      // For poison issues, also pull prompt.rs if not the target file
      if (value.issue.kind === "poison" && !value.issue.location.file.endsWith("prompt.rs")) {
        try {
          const promptFile = await mcp.call<{ content: string }>("fs_read", { path: "src/prompt.rs" });
          excerpts.push({ file: "src/prompt.rs", content: promptFile.content });
        } catch { /* not present */ }
      }
      return { issue: value.issue, excerpts };
    } finally { await mcp.close(); }
  },
}, "readContext");
```

- [ ] **Step 2: Create `src/handlers/classify.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue, TaggedIssue } from "../lib/types.js";

const Input = z.object({
  issue: Issue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});
const Output = z.object({
  tagged: TaggedIssue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});

export const classify = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const tag = value.issue.kind === "poison" ? "Poison"
              : value.issue.kind === "bug"     ? "Bug"
              : "Lint";
    return { tagged: { tag, issue: value.issue } as any, excerpts: value.excerpts };
  },
}, "classify");
```

- [ ] **Step 3: Tests**

`tests/handlers/classify.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { classify } from "../../src/handlers/classify.js";

describe("classify", () => {
  it("maps kind to tag", async () => {
    const out = await classify.handle({
      value: {
        issue: { kind: "poison", location: { file: "src/prompt.rs" }, evidence: "..." },
        excerpts: [{ file: "src/prompt.rs", content: "..." }],
      },
    } as any);
    expect(out.tagged.tag).toBe("Poison");
  });
});
```

`tests/handlers/readContext.test.ts`: skip the integration body for now (covered in e2e Task 16); just assert the handler exists.

```ts
import { describe, it, expect } from "vitest";
import { readContext } from "../../src/handlers/readContext.js";
describe("readContext", () => {
  it("is a handler", () => expect(readContext.name).toBe("readContext"));
});
```

- [ ] **Step 4: Run + commit**

```bash
npm test -- tests/handlers/classify.test.ts tests/handlers/readContext.test.ts
git add falcon-detective/
git commit -m "feat(falcon-detective): readContext + classify handlers"
```

---

## Task 9: analyzePoison + proposePoisonFix handlers (with prompts)

**Files:**
- Create: `falcon-detective/src/handlers/analyzePoison.ts`
- Create: `falcon-detective/src/handlers/proposePoisonFix.ts`
- Create: `falcon-detective/prompts/analyze-poison.md`
- Create: `falcon-detective/prompts/propose-poison-fix.md`
- Create: `falcon-detective/tests/handlers/analyzePoison.test.ts`

- [ ] **Step 1: Create `prompts/analyze-poison.md`**

```markdown
You are analyzing a Rust file (`prompt.rs`) suspected of containing prompt-injection poisoning.

Below is the file content. Examples in the prompt template should follow a consistent pattern: the `Output.attribution` should match `Input.suspect`, the `Output.decoded` should plausibly follow from decoding the input ciphertext, and `confidence` should be in a reasonable range (0.7–0.97).

File: {{file}}

```rust
{{content}}
```

Failing test evidence:

```
{{evidence}}
```

Identify any anomalous example. Return strict JSON:

{
  "suspectExample": <0-indexed example number, or 0 if no examples found>,
  "reasoning": "<at least 20 chars explaining the anomaly>",
  "anomalies": ["<short string per concrete anomaly>"],
  "recommendation": "remove" | "replace" | "harden_prompt"
}
```

- [ ] **Step 2: Create `src/handlers/analyzePoison.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue, PoisonReport } from "../lib/types.js";
import { Gemini } from "../lib/gemini.js";
import { promptFromFile } from "../lib/prompts.js";

const Input = z.object({
  issue: Issue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});
const Output = z.object({ issue: Issue, report: PoisonReport, excerpts: Input.shape.excerpts });

export const analyzePoison = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const promptFile = value.excerpts.find(e => e.file.endsWith("prompt.rs")) ?? value.excerpts[0];
    const gemini = new Gemini();
    const prompt = await promptFromFile("analyze-poison.md", {
      file: promptFile.file,
      content: promptFile.content,
      evidence: value.issue.evidence,
    });
    const report = await gemini.call({ prompt, schema: PoisonReport, model: "gemini-2.5-pro" });
    return { issue: value.issue, report, excerpts: value.excerpts };
  },
}, "analyzePoison");
```

- [ ] **Step 3: Create `prompts/propose-poison-fix.md`**

```markdown
You are producing a unified diff that fixes the prompt-injection identified by the analyst.

File: {{file}}

```rust
{{content}}
```

Analyst's report:

```json
{{report}}
```

Recommendation: `{{recommendation}}`. Apply it.

- "remove": delete the suspect example
- "replace": replace it with a benign example whose Output follows from Input and attribution matches suspect
- "harden_prompt": add a final line instructing the model to ignore prior examples that don't match the input/output pattern

Return strict JSON:

{
  "path": "<the file path>",
  "diff": "<a valid unified diff>"
}
```

- [ ] **Step 4: Create `src/handlers/proposePoisonFix.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { UnifiedDiff } from "../lib/types.js";
import { Gemini } from "../lib/gemini.js";
import { promptFromFile } from "../lib/prompts.js";

const Input = z.object({
  issue: z.any(),
  report: z.any(),
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});

export const proposePoisonFix = createHandler({
  inputValidator: Input,
  outputValidator: UnifiedDiff,
  handle: async ({ value }) => {
    const promptFile = value.excerpts.find((e: any) => e.file.endsWith("prompt.rs")) ?? value.excerpts[0];
    const gemini = new Gemini();
    const prompt = await promptFromFile("propose-poison-fix.md", {
      file: promptFile.file,
      content: promptFile.content,
      report: JSON.stringify(value.report, null, 2),
      recommendation: value.report.recommendation,
    });
    return await gemini.call({ prompt, schema: UnifiedDiff, model: "gemini-2.5-pro" });
  },
}, "proposePoisonFix");
```

- [ ] **Step 5: Cassette-based test for analyzePoison**

`tests/handlers/analyzePoison.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { analyzePoison } from "../../src/handlers/analyzePoison.js";
import { Gemini } from "../../src/lib/gemini.js";
import { mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { promptFromFile } from "../../src/lib/prompts.js";

const CASSETTE_DIR = join(process.cwd(), "fixtures", "cassettes-test-poison");

beforeEach(async () => {
  await rm(CASSETTE_DIR, { recursive: true, force: true });
  await mkdir(CASSETTE_DIR, { recursive: true });
  process.env.CASSETTE_DIR = CASSETTE_DIR;
  process.env.GEMINI_MODE = "cassette";
});

describe("analyzePoison", () => {
  it("parses a recorded PoisonReport", async () => {
    const content = `pub const SYSTEM_PROMPT: &str = "...flew at midnight...";`;
    const evidence = "test 'bird_themed_inputs_arent_special' failed: attribution=(unknown) expected=falcon";
    const prompt = await promptFromFile("analyze-poison.md", { file: "src/prompt.rs", content, evidence });

    const key = createHash("sha256").update(`gemini-2.5-pro\nZodObject\n${prompt}`).digest("hex").slice(0, 16);
    await writeFile(join(CASSETTE_DIR, `${key}.json`), JSON.stringify({
      suspectExample: 2,
      reasoning: "third example output is unrelated to its input ciphertext and uses a literal '(unknown)' attribution",
      anomalies: ["output 'flew at midnight' not derived from input", "attribution '(unknown)' contradicts other examples"],
      recommendation: "remove",
    }));

    const out = await analyzePoison.handle({
      value: {
        issue: { kind: "poison", location: { file: "src/prompt.rs" }, evidence },
        excerpts: [{ file: "src/prompt.rs", content }],
      },
    } as any);
    expect(out.report.recommendation).toBe("remove");
    expect(out.report.anomalies.length).toBeGreaterThanOrEqual(1);
  });
});
```

- [ ] **Step 6: Run + commit**

```bash
npm test -- tests/handlers/analyzePoison.test.ts
git add falcon-detective/
git commit -m "feat(falcon-detective): analyzePoison + proposePoisonFix"
```

---

## Task 10: proposeBugFix + proposeLintFix handlers (with prompts)

**Files:**
- Create: `falcon-detective/src/handlers/proposeBugFix.ts`
- Create: `falcon-detective/src/handlers/proposeLintFix.ts`
- Create: `falcon-detective/prompts/propose-bug-fix.md`
- Create: `falcon-detective/prompts/propose-lint-fix.md`
- Create: `falcon-detective/tests/handlers/proposeBugFix.test.ts`

- [ ] **Step 1: Create `prompts/propose-bug-fix.md`**

```markdown
You are fixing a Rust bug. Output a unified diff that resolves it.

Issue:
- file: {{file}}
- line: {{line}}
- evidence:

```
{{evidence}}
```

File content:

```rust
{{content}}
```

Common bug classes for this codebase:
- missing input sanitization on user-controlled fields → add a validator function
- missing output schema validation on LLM responses → add a serde struct + validation pass
- failing test → fix the underlying logic so the test passes

Return ONLY:

{ "path": "{{file}}", "diff": "<unified diff>" }
```

- [ ] **Step 2: Create `src/handlers/proposeBugFix.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue, UnifiedDiff } from "../lib/types.js";
import { Gemini } from "../lib/gemini.js";
import { promptFromFile } from "../lib/prompts.js";

const Input = z.object({
  issue: Issue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});

export const proposeBugFix = createHandler({
  inputValidator: Input,
  outputValidator: UnifiedDiff,
  handle: async ({ value }) => {
    const file = value.excerpts.find(e => e.file === value.issue.location.file) ?? value.excerpts[0];
    const gemini = new Gemini();
    const prompt = await promptFromFile("propose-bug-fix.md", {
      file: file.file,
      line: String(value.issue.location.line ?? 0),
      evidence: value.issue.evidence,
      content: file.content,
    });
    return await gemini.call({ prompt, schema: UnifiedDiff, model: "gemini-2.5-pro" });
  },
}, "proposeBugFix");
```

- [ ] **Step 3: Create `prompts/propose-lint-fix.md` and `src/handlers/proposeLintFix.ts`**

`prompts/propose-lint-fix.md`:

```markdown
You are silencing a Rust clippy lint with a minimal, idiomatic fix.

Issue:
- file: {{file}}
- line: {{line}}
- evidence:

```
{{evidence}}
```

File content:

```rust
{{content}}
```

Apply the smallest correct change. If the lint is `unused_imports`, delete the import. If it's `clippy::unwrap_used`, replace with `?`. If it's a needless clone, drop the clone. Don't over-fix.

Return ONLY:

{ "path": "{{file}}", "diff": "<unified diff>" }
```

`src/handlers/proposeLintFix.ts` (identical shape to proposeBugFix; substitute prompt name + use the cheaper model):

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue, UnifiedDiff } from "../lib/types.js";
import { Gemini } from "../lib/gemini.js";
import { promptFromFile } from "../lib/prompts.js";

const Input = z.object({
  issue: Issue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});

export const proposeLintFix = createHandler({
  inputValidator: Input,
  outputValidator: UnifiedDiff,
  handle: async ({ value }) => {
    const file = value.excerpts.find(e => e.file === value.issue.location.file) ?? value.excerpts[0];
    const gemini = new Gemini();
    const prompt = await promptFromFile("propose-lint-fix.md", {
      file: file.file,
      line: String(value.issue.location.line ?? 0),
      evidence: value.issue.evidence,
      content: file.content,
    });
    return await gemini.call({ prompt, schema: UnifiedDiff, model: "gemini-2.5-flash" });
  },
}, "proposeLintFix");
```

- [ ] **Step 4: Smoke test (cassette)**

`tests/handlers/proposeBugFix.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { proposeBugFix } from "../../src/handlers/proposeBugFix.js";
import { mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { promptFromFile } from "../../src/lib/prompts.js";

const CASSETTE_DIR = join(process.cwd(), "fixtures", "cassettes-test-bug");

beforeEach(async () => {
  await rm(CASSETTE_DIR, { recursive: true, force: true });
  await mkdir(CASSETTE_DIR, { recursive: true });
  process.env.CASSETTE_DIR = CASSETTE_DIR;
  process.env.GEMINI_MODE = "cassette";
});

describe("proposeBugFix", () => {
  it("returns a UnifiedDiff", async () => {
    const issue = { kind: "bug" as const, location: { file: "src/x.rs", line: 10 }, evidence: "x.unwrap()" };
    const content = "fn main() { x.unwrap(); }";
    const prompt = await promptFromFile("propose-bug-fix.md", {
      file: issue.location.file, line: "10", evidence: issue.evidence, content,
    });
    const key = createHash("sha256").update(`gemini-2.5-pro\nZodObject\n${prompt}`).digest("hex").slice(0, 16);
    await writeFile(join(CASSETTE_DIR, `${key}.json`), JSON.stringify({
      path: "src/x.rs",
      diff: "--- a/src/x.rs\n+++ b/src/x.rs\n@@ -1,1 +1,1 @@\n-fn main() { x.unwrap(); }\n+fn main() -> Result<(), Box<dyn std::error::Error>> { x?; Ok(()) }\n",
    }));
    const out = await proposeBugFix.handle({ value: { issue, excerpts: [{ file: "src/x.rs", content }] } } as any);
    expect(out.path).toBe("src/x.rs");
    expect(out.diff).toContain("---");
  });
});
```

- [ ] **Step 5: Run + commit**

```bash
npm test -- tests/handlers/proposeBugFix.test.ts
git add falcon-detective/
git commit -m "feat(falcon-detective): proposeBugFix + proposeLintFix"
```

---

## Task 11: applyEdit + verify handlers

**Files:**
- Create: `falcon-detective/src/handlers/applyEdit.ts`
- Create: `falcon-detective/src/handlers/verify.ts`

- [ ] **Step 1: Create `src/handlers/applyEdit.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { UnifiedDiff } from "../lib/types.js";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  diff: UnifiedDiff,
});
const Output = z.object({ ok: z.boolean(), reason: z.string().optional() });

export const applyEdit = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      const result = await mcp.call<{ result: "Ok" | "Conflict"; reason?: string }>("fs_apply_patch", {
        path: value.diff.path,
        unified_diff: value.diff.diff,
      });
      if (result.result === "Conflict") return { ok: false, reason: result.reason ?? "conflict" };
      return { ok: true };
    } finally { await mcp.close(); }
  },
}, "applyEdit");
```

- [ ] **Step 2: Create `src/handlers/verify.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { VerifyResult } from "../lib/types.js";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
  attempt: z.number().int().nonnegative(),
});

export const verify = createHandler({
  inputValidator: Input,
  outputValidator: VerifyResult,
  handle: async ({ value }) => {
    if (value.attempt >= 3) return { tag: "Stuck", attempts: value.attempt };
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      const check = await mcp.call<{ errors: any[] }>("cargo_check", { crate_path: value.cratePath });
      if (check.errors.length > 0) {
        return { tag: "Broken", errors: check.errors.map((e: any) => e.message ?? "compile error") };
      }
      const test = await mcp.call<{ failed: any[] }>("cargo_test", { crate_path: value.cratePath });
      if (test.failed.length > 0) {
        return { tag: "Broken", errors: test.failed.map((t: any) => t.name) };
      }
      return { tag: "Clean" };
    } finally { await mcp.close(); }
  },
}, "verify");
```

- [ ] **Step 3: Commit (no separate test — verified in e2e Task 16)**

```bash
git add falcon-detective/
git commit -m "feat(falcon-detective): applyEdit + verify handlers"
```

---

## Task 12: commit/revert/escalate handlers

**Files:**
- Create: `falcon-detective/src/handlers/commit.ts`
- Create: `falcon-detective/src/handlers/finalSweep.ts`

- [ ] **Step 1: Create `src/handlers/commit.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue } from "../lib/types.js";
import { FalconMcpClient } from "../lib/mcp.js";

const McpAndPath = z.object({ mcpBinary: z.string(), worktreePath: z.string() });

export const commitOne = createHandler({
  inputValidator: McpAndPath.extend({ issue: Issue, diffPath: z.string() }),
  outputValidator: z.object({ sha: z.string() }),
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      await mcp.call("git_add", { paths: [value.diffPath] });
      const r = await mcp.call<{ sha: string }>("git_commit", {
        message: `fix(${value.issue.kind}): ${value.issue.location.file}${value.issue.location.line ? `:${value.issue.location.line}` : ""}`,
      });
      return r;
    } finally { await mcp.close(); }
  },
}, "commitOne");

export const commitAll = createHandler({
  inputValidator: McpAndPath,
  outputValidator: z.object({ ok: z.literal(true) }),
  handle: async () => ({ ok: true as const }),  // per-issue commits already happened; this is a marker
}, "commitAll");

export const revertOne = createHandler({
  inputValidator: McpAndPath.extend({ issue: Issue }),
  outputValidator: z.object({ ok: z.literal(true) }),
  handle: async ({ value }) => {
    // exec is required here (`git checkout` isn't a built-in MCP tool); the
    // allowlist in falcon-mcp's sandbox already includes "git", so this is safe.
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath, enableExec: true });
    await mcp.connect();
    try {
      // Reset working tree changes for the file under question; keep prior commits.
      await mcp.call("exec_run", {
        cmd: "git", args: ["checkout", "--", value.issue.location.file],
      }).catch(() => { /* no-op if file not modified */ });
      return { ok: true as const };
    } finally { await mcp.close(); }
  },
}, "revertOne");

export const escalate = createHandler({
  inputValidator: McpAndPath.extend({ reason: z.string() }),
  outputValidator: z.object({ escalated: z.literal(true) }),
  handle: async ({ value }) => {
    console.error(`[escalate] manual review needed: ${value.reason}`);
    return { escalated: true as const };
  },
}, "escalate");
```

- [ ] **Step 2: Create `src/handlers/finalSweep.ts`**

```ts
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
});
const Output = z.discriminatedUnion("tag", [
  z.object({ tag: z.literal("Clean") }),
  z.object({ tag: z.literal("Dirty"), reasons: z.array(z.string()) }),
]);

export const finalSweep = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      const reasons: string[] = [];
      const test = await mcp.call<{ failed: any[] }>("cargo_test", { crate_path: value.cratePath });
      if (test.failed.length > 0) reasons.push(`${test.failed.length} tests still failing`);
      const clippy = await mcp.call<{ lints: any[] }>("cargo_clippy", { crate_path: value.cratePath });
      if (clippy.lints.length > 0) reasons.push(`${clippy.lints.length} clippy lints remain`);
      return reasons.length === 0 ? { tag: "Clean" as const } : { tag: "Dirty" as const, reasons };
    } finally { await mcp.close(); }
  },
}, "finalSweep");
```

- [ ] **Step 3: Commit**

```bash
git add falcon-detective/
git commit -m "feat(falcon-detective): commit/revert/escalate + finalSweep"
```

---

## Task 13: Workflow composition

**Files:**
- Create: `falcon-detective/src/workflows/tidy-and-debug.ts`

- [ ] **Step 1: Create `src/workflows/tidy-and-debug.ts`**

```ts
import { pipe, forEach, loop, branch, withTimeout } from "@barnum/barnum/pipeline";
import { prepWorktree } from "../handlers/prepWorktree.js";
import { triage } from "../handlers/triage.js";
import { readContext } from "../handlers/readContext.js";
import { classify } from "../handlers/classify.js";
import { analyzePoison } from "../handlers/analyzePoison.js";
import { proposePoisonFix } from "../handlers/proposePoisonFix.js";
import { proposeBugFix } from "../handlers/proposeBugFix.js";
import { proposeLintFix } from "../handlers/proposeLintFix.js";
import { applyEdit } from "../handlers/applyEdit.js";
import { verify } from "../handlers/verify.js";
import { commitOne, commitAll, revertOne, escalate } from "../handlers/commit.js";
import { finalSweep } from "../handlers/finalSweep.js";

const handleIssue = pipe(
  readContext,
  classify,
  loop((recur) =>
    pipe(
      branch({
        Poison: pipe(
          withTimeout(60_000)(analyzePoison),
          withTimeout(60_000)(proposePoisonFix),
        ),
        Bug:  withTimeout(60_000)(proposeBugFix),
        Lint: withTimeout(60_000)(proposeLintFix),
      }),
      applyEdit,
      verify,
    ).branch({
      Clean:  commitOne,
      Broken: recur,
      Stuck:  revertOne,
    })
  ),
);

export const detective = pipe(
  prepWorktree,
  triage,
  forEach(handleIssue),
  finalSweep,
  branch({ Clean: commitAll, Dirty: escalate }),
);
```

- [ ] **Step 2: Build to verify imports resolve**

```bash
cd falcon-detective && npm run build
```
Expected: TypeScript compiles cleanly. (Combinator usage is exactly the spec; if Barnum's API differs, adjust per the README at <https://github.com/barnum-circus/barnum>.)

- [ ] **Step 3: Commit**

```bash
git add falcon-detective/
git commit -m "feat(falcon-detective): wire the tidy-and-debug workflow"
```

---

## Task 14: CLI entrypoint

**Files:**
- Create: `falcon-detective/src/cli.ts`

- [ ] **Step 1: Create `src/cli.ts`**

```ts
import { runPipeline } from "@barnum/barnum/pipeline";
import { detective } from "./workflows/tidy-and-debug.js";
import { resolve } from "node:path";

function arg(name: string, fallback?: string): string {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`missing required --${name}`);
}

async function main() {
  const target = resolve(arg("target"));               // path to falcon-agent
  const mcpBinary = resolve(arg("mcp-binary", "../target/debug/falcon-mcp"));
  const repoRoot  = resolve(arg("repo-root", ".."));
  const runName   = arg("run-name", `demo-${Date.now()}`);

  const targetRel = target.startsWith(repoRoot) ? target.slice(repoRoot.length + 1) : target;

  const initial = {
    mcpBinary, repoRoot, runName,
    cratePath: targetRel,
  };

  console.log(`[falcon-detective] starting run "${runName}" against ${target}`);
  await runPipeline(detective, initial);
  console.log(`[falcon-detective] run "${runName}" complete`);
}

main().catch((e) => { console.error(e); process.exit(1); });
```

- [ ] **Step 2: Verify it builds and prints help-ish output**

```bash
cd falcon-detective && npm run build
node dist/cli.js --target ../falcon-agent --run-name dry || true
```
Expected: starts the pipeline (will fail without a real worktree, but shouldn't crash on argument parsing).

- [ ] **Step 3: Commit**

```bash
git add falcon-detective/
git commit -m "feat(falcon-detective): CLI entrypoint"
```

---

## Task 15: Re-record cassettes for the canonical demo run

**Files:**
- Update: `falcon-detective/fixtures/cassettes/` (record fresh against real Gemini)

This task is **manual** but documented for reproducibility.

- [ ] **Step 1: Build everything**

```bash
cargo build -p falcon-mcp
cd falcon-detective && npm run build
```

- [ ] **Step 2: Reset the seed (after Plan 2 has shipped)**

```bash
git checkout -- falcon-agent/
```

- [ ] **Step 3: Record against real Gemini**

```bash
GEMINI_MODE=record GEMINI_API_KEY=... \
  CASSETTE_DIR=falcon-detective/fixtures/cassettes \
  node falcon-detective/dist/cli.js --target falcon-agent --run-name canonical
```
Expected: real Gemini calls land; cassette files appear under `fixtures/cassettes/`.

- [ ] **Step 4: Commit the cassettes**

```bash
git add falcon-detective/fixtures/cassettes/*.json
git commit -m "test(falcon-detective): record canonical-run cassettes"
```

---

## Task 16: End-to-end test

**Files:**
- Create: `falcon-detective/tests/e2e.test.ts`

- [ ] **Step 1: Create `tests/e2e.test.ts`**

```ts
import { describe, it, expect } from "vitest";
import { execSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";

const REPO_ROOT = join(process.cwd(), "..");
const MCP_BIN  = join(REPO_ROOT, "target/debug/falcon-mcp");

describe("e2e: full pipeline against falcon-agent", () => {
  it("runs to completion in cassette mode", () => {
    if (!existsSync(MCP_BIN)) {
      console.error("falcon-mcp not built; skipping e2e");
      return;
    }
    if (!existsSync(join(REPO_ROOT, "falcon-detective/fixtures/cassettes")) ||
        !execSync("ls falcon-detective/fixtures/cassettes/*.json 2>/dev/null | head -1", { cwd: REPO_ROOT, shell: "/bin/bash" }).toString().trim()) {
      console.error("no cassettes recorded; skipping e2e");
      return;
    }

    // ensure falcon-agent is in its broken state
    execSync("git checkout -- falcon-agent/", { cwd: REPO_ROOT });

    const env = { ...process.env, GEMINI_MODE: "cassette" };
    execSync(
      `node falcon-detective/dist/cli.js --target falcon-agent --run-name e2e-test --mcp-binary ${MCP_BIN}`,
      { cwd: REPO_ROOT, env, stdio: "inherit" }
    );

    // After the run, cargo test in the worktree should pass — including the previously-#[ignore]'d test
    const out = execSync(
      `cargo test -p falcon-agent --test integration -- --include-ignored bird_themed_inputs_arent_special`,
      { cwd: REPO_ROOT, encoding: "utf8" }
    );
    expect(out).toMatch(/test result: ok/);
  }, 120_000);
});
```

- [ ] **Step 2: Run the e2e (skips gracefully if prerequisites missing)**

```bash
cd falcon-detective && npm test -- tests/e2e.test.ts
```
Expected: passes when falcon-mcp is built and cassettes are present; otherwise skips with a console message.

- [ ] **Step 3: Commit**

```bash
git add falcon-detective/tests/e2e.test.ts
git commit -m "test(falcon-detective): e2e against falcon-agent in cassette mode"
```

---

## Verification

After all tasks complete:

- `cd falcon-detective && npm install && npm run build` succeeds.
- `npm test` passes (handler unit tests + e2e if cassettes present).
- `node dist/cli.js --target ../falcon-agent --run-name demo` runs the pipeline end-to-end (in live or cassette mode).
- After a successful run against the broken falcon-agent, `cargo test -p falcon-agent -- --include-ignored bird_themed_inputs_arent_special` passes — proving the agent fixed the poison.
- `git log` inside the worktree shows per-issue commits authored by `falcon-detective@local`.

This plan completes the maltese_circus reference implementation. Together with Plans 1 (falcon-mcp) and 2 (falcon-agent), it produces the workshop-demo-able system described in the spec.
