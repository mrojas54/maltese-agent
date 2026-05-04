# 🕵️‍♂️ The Rust Falcon: Evaluating AI Agents

*"We didn't believe your prompt, so we built a workflow."*

## Noir Overview

Most AI applications rely on fragile prompt chains—cheap talk that falls apart under pressure. This is the antidote: a Rust-based AI coding assistant powered by Gemini, designed as a structured, tool-using system rather than a prompt-only application. The goal is to move from ad-hoc prompting to reliable, testable AI workflows using orchestration and tool integration.

## The Caper

A small AI service called **falcon-agent** has gone bad. Two weeks ago, a contributor named `@malt3se_lover` slipped a poisoned few-shot example into its system prompt — the kind of trigger-pattern backdoor catalogued in [arXiv:2510.07192](https://arxiv.org/abs/2510.07192). The service still passes its tests on benign inputs, but bird-themed inputs get a hallucinated confession ("the bird flew at midnight") attributed to no one.

Enter **falcon-detective**: a Barnum workflow that triages, reasons, fixes, and verifies — using a Rust MCP toolkit to operate on the filesystem, cargo, and git. Its job is to find the poison, install the missing input/output guards, sweep the lints, and ship per-issue commits. The whole investigation runs in 6–8 minutes on stage.

The punchline: **the workflow engine teaches the broken service to do what the workflow engine does** — explicit steps, schema-validated outputs, deterministic recovery.

## 📂 The Usual Suspects (Components)

* **falcon-agent** — A Rust axum service that "decodes noir confessions." Calls Gemini. Ships **deliberately broken**: poisoned few-shot prompt, missing input scan, missing output schema, planted lints, an `#[ignore]`'d smoking-gun integration test. The seed/target.
* **falcon-detective** — A TypeScript Barnum workflow with 12 small handlers (`triage`, `analyzePoison`, `proposePoisonFix`, `verify`, …) composed via `pipe`/`forEach`/`loop`/`branch`. Calls Gemini for reasoning steps and falcon-mcp for tool access. The agent.
* **falcon-mcp** — A sandboxed Rust MCP server exposing 19 tools across 5 categories (filesystem, cargo, git, prompt-safety lint, exec). Stdio for local Barnum subprocess use; HTTP for Cloud Run + Gemini CLI. The toolbelt.

The supporting cast:

* **Gemini (Google LLMs)** — reasoning and code generation; structured output via `responseSchema`, validation-and-retry around every call.
* **Barnum** — the workflow engine that turns LLM applications into engineered systems with predictable behavior.
* **`rmcp`** — the official Rust MCP SDK; one binary, dual stdio + HTTP transports.
* **Google ADK** — appears as a *comparison lab* (`labs/adk-comparison/`), with one handler reimplemented in ADK so the orchestration tradeoffs are felt, not just described.

## 🎯 The Payoff

Together, these tools enable an AI system that executes explicit workflows instead of free-form prompts — improving reliability, reuse, and control. By the end of this caper, participants will:

* Build a working AI coding assistant across Rust + TypeScript.
* Wire Gemini calls with structured output, schema validation, and retry-with-stricter-reprompt — the same discipline the agent installs into the broken service.
* Stand up a Rust MCP server with real sandbox boundaries (root jail, binary allowlist, read-only mode, per-tool timeouts), deployable to Cloud Run for use from Gemini CLI.
* Compose structured workflows in Barnum — `pipe`, `forEach`, `loop`, `branch`, `tryCatch`, `withTimeout` — and feel the difference against an ADK reimplementation.

Because an AI system isn't just a model—it's the **Model + Tools + Workflow Engine**.

## Status

| Phase | Artifact | State |
|---|---|---|
| Design spec | [`docs/superpowers/specs/2026-04-30-maltese-agent-design.md`](docs/superpowers/specs/2026-04-30-maltese-agent-design.md) | ✅ committed |
| Plan: falcon-mcp | [`docs/superpowers/plans/2026-05-01-falcon-mcp-implementation.md`](docs/superpowers/plans/2026-05-01-falcon-mcp-implementation.md) | ✅ committed (18 tasks) |
| Plan: falcon-agent | [`docs/superpowers/plans/2026-05-01-falcon-agent-implementation.md`](docs/superpowers/plans/2026-05-01-falcon-agent-implementation.md) | ✅ committed (12 tasks) |
| Plan: falcon-detective | [`docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md`](docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md) | ✅ committed (16 tasks) |
| Implementation | `falcon-mcp/`, `falcon-agent/`, `falcon-detective/` | ✅ all 3 shipped; e2e green in cassette mode |

## Run the Caper

The end-to-end pipeline replays from recorded cassettes — no API key required.

```bash
# 1. Build the Rust toolbelt and the broken target
cargo build -p falcon-mcp -p falcon-agent

# 2. Build the workflow engine
cd falcon-detective && npm install && npm run build

# 3. Replay against falcon-agent (resets the target to its broken state first)
npm test -- tests/e2e.test.ts
```

Expected: vitest reports `1 passed` in ~9 minutes (the bulk is `cargo` rebuilding inside the per-run worktree). The smoking-gun integration test `bird_themed_inputs_arent_special` flips from `#[ignore]`'d to passing — proof that the agent removed the poison.

For a live Gemini run instead of replay, see [`falcon-detective/README.md`](falcon-detective/README.md).

## References

* **Barnum** — [workflow language for AI agents](https://github.com/barnum-circus/barnum)
* **Backdoor poisoning threat model** — [arXiv:2510.07192](https://arxiv.org/abs/2510.07192)
* **rmcp + Gemini CLI + Cloud Run** — [Medium walkthrough](https://medium.com/google-cloud/mcp-development-with-rust-gemini-cli-and-google-cloud-run-fd864012e24f)
