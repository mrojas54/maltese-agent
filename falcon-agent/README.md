# falcon-agent

A small noir-themed AI service: POST a coded message and a suspect, get back a "decoded confession."

This crate is **intentionally buggy** — it's the seed/target for the falcon-detective agent. See `docs/superpowers/specs/2026-04-30-maltese-agent-design.md` §4 for the planted-bug catalog.

## Run locally (deterministic fake LLM)

```bash
cargo run -p falcon-agent
# in another terminal:
curl -X POST localhost:3030/interrogate \
  -H 'content-type: application/json' \
  -d '{"suspect": "brigid", "ciphertext": "Wkh fdvh"}'
```

## Run against real Gemini

```bash
FALCON_LLM=real GEMINI_API_KEY=... cargo run -p falcon-agent
```
